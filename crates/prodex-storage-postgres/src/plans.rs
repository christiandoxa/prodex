use super::*;

pub fn plan_postgres_atomic_reservation(
    command: AtomicReservationCommand,
) -> Result<PostgresAtomicReservationSqlPlan, PostgresStoragePlanError> {
    validate_postgres_reservation_inputs(
        command.storage_key,
        &command.idempotency_key,
        command.limit,
        command.request,
    )?;
    Ok(PostgresAtomicReservationSqlPlan {
        tenant_id: command.request.tenant_id,
        storage_key: command.storage_key,
        idempotency_key: command.idempotency_key,
        statements: postgres_request_path_statements(ATOMIC_RESERVATION_STATEMENT),
    })
}

pub fn plan_postgres_expired_reservation_recovery(
    command: ExpiredReservationRecoveryCommand,
) -> Result<PostgresExpiredReservationRecoverySqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_expired_reservation_recovery(command).map_err(
            |error| match error {
                prodex_storage::ExpiredReservationRecoveryPlanError::TenantMismatch {
                    key_tenant,
                    record_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: record_tenant,
                },
                prodex_storage::ExpiredReservationRecoveryPlanError::NotExpired => {
                    PostgresStoragePlanError::ReservationNotExpired
                }
                prodex_storage::ExpiredReservationRecoveryPlanError::InvalidRecord => {
                    PostgresStoragePlanError::InvalidReservationRecord
                }
            },
        )?;
    Ok(PostgresExpiredReservationRecoverySqlPlan {
        tenant_id: plan.ledger_event.tenant_id,
        storage_key: plan.storage_key,
        statements: postgres_request_path_statements(RECOVER_EXPIRED_RESERVATION_STATEMENT),
    })
}

pub fn plan_postgres_usage_reconciliation(
    command: UsageReconciliationCommand,
) -> Result<PostgresUsageReconciliationSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_usage_reconciliation(command).map_err(|error| match error {
        prodex_storage::UsageReconciliationPlanError::TenantMismatch {
            key_tenant,
            record_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: record_tenant,
        },
        prodex_storage::UsageReconciliationPlanError::ActualExceedsReserved {
            reserved,
            actual,
        } => PostgresStoragePlanError::ActualUsageExceedsReserved { reserved, actual },
        prodex_storage::UsageReconciliationPlanError::CommittedUsageOverflow {
            committed,
            actual,
        } => PostgresStoragePlanError::CommittedUsageOverflow { committed, actual },
        prodex_storage::UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        } => PostgresStoragePlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        },
    })?;
    Ok(PostgresUsageReconciliationSqlPlan {
        tenant_id: plan.reconciliation.commit.tenant_id,
        storage_key: plan.storage_key,
        ledger_event_count: plan.ledger_events.len(),
        statements: postgres_request_path_statements(RECONCILE_USAGE_STATEMENT),
    })
}

pub fn plan_postgres_billing_ledger_query(
    command: BillingLedgerQueryCommand,
) -> Result<PostgresBillingLedgerQuerySqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_billing_ledger_query(command).map_err(|error| match error {
        prodex_storage::BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
        prodex_storage::BillingLedgerQueryPlanError::StartAfterEnd { .. } => {
            PostgresStoragePlanError::InvalidTimeRange
        }
    })?;
    let statement = match plan.sort_order {
        BillingLedgerSortOrder::OccurredAtAsc => QUERY_BILLING_LEDGER_ASC_STATEMENT,
        BillingLedgerSortOrder::OccurredAtDesc => QUERY_BILLING_LEDGER_DESC_STATEMENT,
    };
    Ok(PostgresBillingLedgerQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: postgres_request_path_statements(statement),
    })
}

pub fn plan_postgres_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<PostgresAppendOnlyAuditSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_append_only_audit(command).map_err(|error| match error {
        prodex_storage::AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant,
            event_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: event_tenant,
        },
    })?;
    Ok(PostgresAppendOnlyAuditSqlPlan {
        tenant_id: plan.envelope.event.tenant_id,
        storage_key: plan.storage_key,
        statements: postgres_request_path_statements(APPEND_AUDIT_STATEMENT),
    })
}

pub fn plan_postgres_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<PostgresAuditRetentionPurgeSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_audit_retention_purge(command).map_err(|error| match error {
            prodex_storage::AuditRetentionPurgePlanError::TenantMismatch {
                key_tenant,
                batch_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: batch_tenant,
            },
        })?;
    Ok(PostgresAuditRetentionPurgeSqlPlan {
        tenant_id: plan.batch.tenant_id,
        storage_key: plan.storage_key,
        event_count: plan.batch.len(),
        statements: postgres_request_path_statements(PURGE_AUDIT_RETENTION_STATEMENT),
    })
}

pub fn plan_postgres_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<PostgresAuditExportQuerySqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_audit_export_query(command).map_err(|error| match error {
        prodex_storage::AuditExportQueryPlanError::TenantMismatch {
            key_tenant,
            query_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant: query_tenant,
        },
    })?;
    let statement = match plan.sort_order {
        AuditSortOrder::OccurredAtAsc => QUERY_AUDIT_EXPORT_ASC_STATEMENT,
        AuditSortOrder::OccurredAtDesc => QUERY_AUDIT_EXPORT_DESC_STATEMENT,
    };
    Ok(PostgresAuditExportQuerySqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        sort_order: plan.sort_order,
        page_limit: plan.page_limit,
        statements: postgres_request_path_statements(statement),
    })
}

pub fn plan_postgres_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<PostgresRoleBindingMutationSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_role_binding_mutation(command).map_err(|error| match error {
            prodex_storage::RoleBindingMutationPlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    let operation = match plan.kind {
        RoleBindingMutationKind::Grant => GRANT_ROLE_BINDING_STATEMENT,
        RoleBindingMutationKind::Revoke => REVOKE_ROLE_BINDING_STATEMENT,
    };
    Ok(PostgresRoleBindingMutationSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        role_binding_id: plan.role_binding_id,
        role: plan.role,
        kind: plan.kind,
        statements: postgres_request_path_statements(operation),
    })
}

pub fn plan_postgres_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<PostgresServiceIdentityCreateSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_service_identity_create(command).map_err(|error| match error {
            prodex_storage::ServiceIdentityCreatePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant,
            },
        })?;
    Ok(PostgresServiceIdentityCreateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        display_name: plan.display_name,
        statements: postgres_request_path_statements(UPSERT_SERVICE_IDENTITY_STATEMENT),
    })
}

pub fn plan_postgres_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<PostgresUserLifecycleSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_user_lifecycle(command).map_err(|error| match error {
        prodex_storage::UserLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::UserLifecyclePlanError::EmptyExternalId => {
            PostgresStoragePlanError::EmptyUserExternalId
        }
        prodex_storage::UserLifecyclePlanError::EmptyDisplayName => {
            PostgresStoragePlanError::EmptyUserDisplayName
        }
    })?;
    let operation = match plan.kind {
        UserLifecycleKind::Create | UserLifecycleKind::Update => UPSERT_USER_LIFECYCLE_STATEMENT,
        UserLifecycleKind::Delete => DELETE_USER_LIFECYCLE_STATEMENT,
    };
    Ok(PostgresUserLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        principal_id: plan.principal_id,
        external_id: plan.external_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(operation),
    })
}

pub fn plan_postgres_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<PostgresBudgetPolicyUpdateSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_budget_policy_update(command).map_err(|error| match error {
        prodex_storage::BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::BudgetPolicyUpdatePlanError::EmptyBudgetScope => {
            PostgresStoragePlanError::EmptyBudgetScope
        }
    })?;
    Ok(PostgresBudgetPolicyUpdateSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        budget_scope: plan.budget_scope,
        limit: plan.limit,
        statements: postgres_request_path_statements(UPSERT_BUDGET_POLICY_STATEMENT),
    })
}

pub fn plan_postgres_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<PostgresTenantLifecycleSqlPlan, PostgresStoragePlanError> {
    let plan = prodex_storage::plan_tenant_lifecycle(command).map_err(|error| match error {
        prodex_storage::TenantLifecyclePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        } => PostgresStoragePlanError::TenantMismatch {
            key_tenant,
            request_tenant,
        },
        prodex_storage::TenantLifecyclePlanError::EmptyDisplayName => {
            PostgresStoragePlanError::EmptyTenantDisplayName
        }
    })?;
    Ok(PostgresTenantLifecycleSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(UPSERT_TENANT_LIFECYCLE_STATEMENT),
    })
}

pub fn plan_postgres_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<PostgresVirtualKeySecretReferenceSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_virtual_key_secret_reference(command).map_err(
            |error| match error {
                prodex_storage::VirtualKeySecretReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
                prodex_storage::VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                } => PostgresStoragePlanError::VirtualKeyMismatch {
                    key_virtual_key_id,
                    request_virtual_key_id,
                },
            },
        )?;
    Ok(PostgresVirtualKeySecretReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        virtual_key_id: plan.virtual_key_id,
        display_name: plan.display_name,
        kind: plan.kind,
        statements: postgres_request_path_statements(UPSERT_VIRTUAL_KEY_SECRET_REFERENCE_STATEMENT),
    })
}

pub fn plan_postgres_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<PostgresProviderCredentialReferenceSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_provider_credential_reference(command).map_err(
            |error| match error {
                prodex_storage::ProviderCredentialReferencePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant,
                },
            },
        )?;
    Ok(PostgresProviderCredentialReferenceSqlPlan {
        tenant_id: plan.tenant_id,
        storage_key: plan.storage_key,
        provider_credential_id: plan.provider_credential_id,
        provider_name: plan.provider_name,
        statements: postgres_request_path_statements(
            UPSERT_PROVIDER_CREDENTIAL_REFERENCE_STATEMENT,
        ),
    })
}

pub fn plan_postgres_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<PostgresIdempotencyPendingRecordSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_pending_record(command).map_err(|error| match error {
            prodex_storage::IdempotencyPendingRecordPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(PostgresIdempotencyPendingRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        statements: postgres_request_path_statements(INSERT_IDEMPOTENCY_PENDING_STATEMENT),
    })
}

pub fn plan_postgres_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<PostgresIdempotencyCompletedRecordSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_completed_record(command).map_err(
            |error| match error {
                prodex_storage::IdempotencyCompletedRecordPlanError::TenantMismatch {
                    key_tenant,
                    operation_tenant,
                } => PostgresStoragePlanError::TenantMismatch {
                    key_tenant,
                    request_tenant: operation_tenant,
                },
            },
        )?;
    let operation = plan.entry.operation();
    if operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    let response_byte_count = match &plan.entry {
        prodex_domain::IdempotencyEntry::Completed(record) => record.response.len(),
        prodex_domain::IdempotencyEntry::Pending { .. } => 0,
    };
    Ok(PostgresIdempotencyCompletedRecordSqlPlan {
        tenant_id: operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: operation.key.clone(),
        response_byte_count,
        statements: postgres_request_path_statements(COMPLETE_IDEMPOTENCY_RECORD_STATEMENT),
    })
}

pub fn plan_postgres_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<PostgresIdempotencyRecordLookupSqlPlan, PostgresStoragePlanError> {
    let plan =
        prodex_storage::plan_idempotency_record_lookup(command).map_err(|error| match error {
            prodex_storage::IdempotencyRecordLookupPlanError::TenantMismatch {
                key_tenant,
                operation_tenant,
            } => PostgresStoragePlanError::TenantMismatch {
                key_tenant,
                request_tenant: operation_tenant,
            },
        })?;
    if plan.operation.key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(PostgresIdempotencyRecordLookupSqlPlan {
        tenant_id: plan.operation.tenant_id,
        storage_key: plan.storage_key,
        idempotency_key: plan.operation.key,
        statements: postgres_request_path_statements(LOOKUP_IDEMPOTENCY_RECORD_STATEMENT),
    })
}

fn postgres_request_path_statements(operation: PostgresStatement) -> Vec<PostgresStatement> {
    vec![SET_TENANT_STATEMENT, operation]
}

fn validate_postgres_reservation_inputs(
    storage_key: TenantStorageKey,
    idempotency_key: &IdempotencyKey,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<(), PostgresStoragePlanError> {
    if storage_key.tenant_id != request.tenant_id {
        return Err(PostgresStoragePlanError::TenantMismatch {
            key_tenant: storage_key.tenant_id,
            request_tenant: request.tenant_id,
        });
    }
    if request.estimate.exceeds(limit.max) {
        return Err(PostgresStoragePlanError::ReservationExceedsLimit {
            requested: request.estimate,
            limit: limit.max,
        });
    }
    if idempotency_key.as_str().trim().is_empty() {
        return Err(PostgresStoragePlanError::EmptyIdempotencyKey);
    }
    Ok(())
}

pub fn statement_contains_ddl(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        "create table",
        "alter table",
        "create policy",
        "drop table",
        "truncate table",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
