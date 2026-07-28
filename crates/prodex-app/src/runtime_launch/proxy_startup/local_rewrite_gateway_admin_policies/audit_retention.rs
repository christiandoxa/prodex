use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_retention_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    base_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let holds_path = format!("{base_path}/holds");
    let purge_path = format!("{base_path}/purge");
    let tenant_id = base_action.tenant.tenant_id;

    if path == holds_path && captured.method.eq_ignore_ascii_case("GET") {
        return match repository.list_audit_legal_holds(tenant_id) {
            Ok(holds) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "governance.audit_legal_hold.list",
                    "data": holds.into_iter().map(audit_legal_hold_json).collect::<Vec<_>>(),
                }),
            ),
            Err(error) => repository_error(error),
        };
    }

    if path == holds_path && captured.method.eq_ignore_ascii_case("POST") {
        let body = match runtime_gateway_admin_json_body(captured) {
            Ok(body) => body,
            Err(response) => return response,
        };
        let event_id = match body
            .get("audit_event_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| AuditEventId::from_str(value).ok())
        {
            Some(event_id) => event_id,
            None => return invalid_request(),
        };
        let reason_code = match body
            .get("reason_code")
            .and_then(serde_json::Value::as_str)
            .map(AuditReasonCode::new)
        {
            Some(Ok(reason_code)) => reason_code,
            _ => return invalid_request(),
        };
        let expires_at = match body.get("expires_at_unix_ms") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => match value
                .as_u64()
                .filter(|expires| *expires > runtime_gateway_now_unix_ms())
                .and_then(|expires| AuditTimestamp::new(expires).ok())
            {
                Some(expires_at) => Some(expires_at),
                None => return invalid_request(),
            },
        };
        let execution = match execution(captured, path, admin_auth, base_action) {
            Ok(execution) => execution,
            Err(response) => return response,
        };
        let hold = AuditRetentionHold::new(
            TenantContext { tenant_id },
            event_id,
            reason_code,
            expires_at,
        );
        let audit = match control_plane_audit_command(
            repository,
            &execution.authorized_action,
            "governance.audit_legal_hold.upsert",
            "audit_legal_hold",
            Some(&event_id.to_string()),
        ) {
            Ok(audit) => audit,
            Err(error) => return repository_error(error),
        };
        return match repository.upsert_audit_legal_hold_idempotent(
            &hold,
            execution.authorized_action.audit_event.principal_id,
            execution.atomic_write.completed_at_unix_ms,
            audit,
            GovernanceMutationIdempotency {
                operation: execution.atomic_write.operation,
                started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
            },
        ) {
            Ok(()) => runtime_gateway_admin_json_response(200, audit_legal_hold_json(hold)),
            Err(error) => repository_error(error),
        };
    }

    if let Some(event_id) = path.strip_prefix(&(holds_path.clone() + "/")) {
        if !captured.method.eq_ignore_ascii_case("DELETE") {
            return build_runtime_proxy_json_error_response(
                405,
                "control_plane_method_not_allowed",
                "HTTP method is not allowed for this legal-hold route",
            );
        }
        let event_id = match AuditEventId::from_str(event_id) {
            Ok(event_id) => event_id,
            Err(_) => return invalid_request(),
        };
        let execution = match super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution(
            captured,
            path,
            admin_auth,
            base_action,
            ControlPlaneOperation::AuditRetentionPurge,
        ) {
            Ok(execution) => execution,
            Err(response) => return response,
        };
        let audit = match control_plane_audit_command(
            repository,
            &execution.authorized_action,
            "governance.audit_legal_hold.delete",
            "audit_legal_hold",
            Some(&event_id.to_string()),
        ) {
            Ok(audit) => audit,
            Err(error) => return repository_error(error),
        };
        return match repository.delete_audit_legal_hold_idempotent(
            tenant_id,
            event_id,
            audit,
            GovernanceMutationIdempotency {
                operation: execution.atomic_write.operation,
                started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
            },
        ) {
            Ok(true) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "governance.audit_legal_hold.deleted",
                    "audit_event_id": event_id,
                }),
            ),
            Ok(false) => repository_error(GovernanceRepositoryError::NotFound),
            Err(error) => repository_error(error),
        };
    }

    if path == purge_path && captured.method.eq_ignore_ascii_case("DELETE") {
        return audit_retention_purge_response(
            captured,
            path,
            shared,
            admin_auth,
            base_action,
            repository,
        );
    }

    if path == holds_path || path == purge_path {
        build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this audit-retention route",
        )
    } else {
        build_runtime_proxy_json_error_response(
            404,
            "audit_retention_not_found",
            "audit-retention resource was not found",
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_retention_purge_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let approval_id = match body
        .get("approval_id")
        .and_then(serde_json::Value::as_str)
        .map(|value| ApprovalId::new(value.to_string()))
    {
        Some(Ok(approval_id)) => approval_id,
        _ => return invalid_request(),
    };
    let mut event_ids = match body
        .get("audit_event_ids")
        .and_then(serde_json::Value::as_array)
    {
        Some(values)
            if !values.is_empty() && values.len() <= usize::from(AuditRetentionBatchLimit::MAX) =>
        {
            let mut ids = Vec::with_capacity(values.len());
            for value in values {
                let Some(value) = value.as_str() else {
                    return invalid_request();
                };
                let Ok(event_id) = AuditEventId::from_str(value) else {
                    return invalid_request();
                };
                ids.push(event_id);
            }
            ids
        }
        _ => return invalid_request(),
    };
    event_ids.sort_unstable();
    event_ids.dedup();
    let retention_days = match body.get("retention_days") {
        None => None,
        Some(value) => match value.as_u64().and_then(|value| u16::try_from(value).ok()) {
            Some(value) => Some(value),
            None => return invalid_request(),
        },
    };
    let retention_policy = match AuditRetentionPolicy::new(retention_days) {
        Ok(policy) => policy,
        Err(_) => return invalid_request(),
    };
    let batch_limit = match AuditRetentionBatchLimit::new(u16::try_from(event_ids.len()).ok()) {
        Ok(limit) => limit,
        Err(_) => return invalid_request(),
    };
    let scope = AuditQueryScope::tenant(TenantContext {
        tenant_id: base_action.tenant.tenant_id,
    });
    let keys = event_ids
        .iter()
        .copied()
        .map(|event_id| AuditRetentionPurgeKey {
            tenant_id: base_action.tenant.tenant_id,
            event_id,
        });
    let batch = match AuditRetentionPurgeBatch::new(scope, keys, batch_limit) {
        Ok(batch) => batch,
        Err(_) => return invalid_request(),
    };
    let durable_store = match shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return storage_unavailable();
        }
    };
    if plan_application_audit_retention_purge(ApplicationAuditRetentionPurgeRequest {
        durable_store,
        purge: AuditRetentionPurgeCommand {
            storage_key: TenantStorageKey::tenant(base_action.tenant.tenant_id),
            batch,
        },
    })
    .is_err()
    {
        return invalid_request();
    }

    let execution =
        match super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution(
            captured,
            path,
            admin_auth,
            base_action,
            ControlPlaneOperation::AuditRetentionPurge,
        ) {
            Ok(execution) => execution,
            Err(response) => return response,
        };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let approval = match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval)
            if approval.kind == ApprovalKind::BreakGlass
                && approval.state == prodex_domain::ApprovalState::Active =>
        {
            approval
        }
        Ok(_) => return break_glass_denied(),
        Err(GovernanceRepositoryError::NotFound) => return break_glass_denied(),
        Err(error) => return repository_error(error),
    };
    let Some(reason) = approval.scope.as_str().strip_prefix("audit_retention:") else {
        return break_glass_denied();
    };
    let http = runtime_gateway_http_request_meta(captured, path);
    let Some(mut action) = runtime_gateway_admin_control_plane_action_for_operation(
        &http,
        admin_auth,
        ControlPlaneOperation::AuditRetentionPurge,
    ) else {
        return invalid_request();
    };
    action.principal = Principal::new(
        action.principal.id,
        Some(tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    );
    action.resource.kind = ResourceKind::AuditLog;
    let authorization = BreakGlassAuthorization {
        reason: reason.to_string(),
        expires_at_unix_ms: approval.expires_at_unix_ms,
    };
    let previous_digest = match repository.latest_audit_digest(tenant_id) {
        Ok(previous_digest) => previous_digest,
        Err(error) => return repository_error(error),
    };
    let preview =
        prodex_control_plane::decide_break_glass_action(action.clone(), authorization.clone());
    let preview_event = match &preview {
        ControlPlaneDecision::Authorized(plan) => &plan.audit_event,
        ControlPlaneDecision::Denied { audit_event, .. } => audit_event,
    };
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), preview_event);
    let plan =
        match plan_application_break_glass_with_audit_storage(ApplicationBreakGlassAuditRequest {
            durable_store,
            action,
            authorization,
            previous_digest,
            event_digest,
        }) {
            Ok(plan) => plan,
            Err(_) => return storage_unavailable(),
        };
    let (authorized, audit_event) = match plan.decision {
        ControlPlaneDecision::Authorized(plan) => (true, plan.audit_event),
        ControlPlaneDecision::Denied { audit_event, .. } => (false, audit_event),
    };
    if super::local_rewrite_governance_audit::persist_runtime_control_plane_audit_event(
        shared,
        audit_event,
    )
    .is_err()
    {
        return storage_unavailable();
    }
    if !authorized {
        return break_glass_denied();
    }

    let now_unix_ms = execution.atomic_write.completed_at_unix_ms;
    let cutoff_unix_ms =
        now_unix_ms.saturating_sub(u64::from(retention_policy.days()).saturating_mul(86_400_000));
    let audit = match control_plane_audit_command(
        repository,
        &execution.authorized_action,
        "governance.audit_retention.purge",
        "audit_log",
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match repository.purge_audit_events_idempotent(
        tenant_id,
        &event_ids,
        now_unix_ms,
        cutoff_unix_ms,
        audit,
        GovernanceMutationIdempotency {
            operation: execution.atomic_write.operation,
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(purged) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "governance.audit_retention_purge",
                "requested": event_ids.len(),
                "purged": purged.len(),
                "protected_or_ineligible": event_ids.len().saturating_sub(purged.len()),
                "audit_event_ids": purged,
                "retention_days": retention_policy.days(),
                "approval_id": approval_id.as_str(),
            }),
        ),
        Err(error) => repository_error(error),
    }
}

fn audit_legal_hold_json(hold: AuditRetentionHold) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.audit_legal_hold",
        "audit_event_id": hold.event_id,
        "reason_code": hold.reason_code.as_str(),
        "expires_at_unix_ms": hold.expires_at.map(AuditTimestamp::unix_ms),
    })
}

fn break_glass_denied() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        403,
        "break_glass_not_authorized",
        "active break-glass approval is required for audit retention purge",
    )
}
