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
    let context = AuditRetentionResponseContext {
        captured,
        path,
        admin_auth,
        base_action,
        repository,
    };
    if path == holds_path {
        return audit_retention_holds_response(&context);
    }
    if let Some(event_id) = path.strip_prefix(&format!("{holds_path}/")) {
        return audit_retention_hold_response(&context, event_id);
    }
    if path == purge_path && captured.method.eq_ignore_ascii_case("DELETE") {
        return audit_retention_purge_response(&context, shared);
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

struct AuditRetentionResponseContext<'a> {
    captured: &'a RuntimeProxyRequest,
    path: &'a str,
    admin_auth: &'a RuntimeGatewayAdminAuth,
    base_action: &'a ControlPlaneActionPlan,
    repository: &'a RuntimeGovernanceRepository<'a>,
}

fn audit_retention_holds_response(
    context: &AuditRetentionResponseContext<'_>,
) -> tiny_http::ResponseBox {
    if context.captured.method.eq_ignore_ascii_case("GET") {
        let holds = match context
            .repository
            .list_audit_legal_holds(context.base_action.tenant.tenant_id)
        {
            Ok(holds) => holds,
            Err(error) => return repository_error(error),
        };
        if let Err(error) = append_control_plane_audit_command(
            context.repository,
            context.base_action,
            "governance.audit_legal_hold.read",
            "audit_legal_hold",
            None,
        ) {
            return repository_error(error);
        }
        return runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "governance.audit_legal_hold.list",
                "data": holds.into_iter().map(audit_legal_hold_json).collect::<Vec<_>>(),
            }),
        );
    }
    if context.captured.method.eq_ignore_ascii_case("POST") {
        return audit_retention_hold_create_response(context);
    }
    build_runtime_proxy_json_error_response(
        405,
        "control_plane_method_not_allowed",
        "HTTP method is not allowed for this legal-hold route",
    )
}

fn audit_retention_hold_create_response(
    context: &AuditRetentionResponseContext<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(context.captured) {
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
    let execution = match execution(
        context.captured,
        context.path,
        context.admin_auth,
        context.base_action,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let hold = AuditRetentionHold::new(
        TenantContext {
            tenant_id: context.base_action.tenant.tenant_id,
        },
        event_id,
        reason_code,
        expires_at,
    );
    let audit = match control_plane_audit_command(
        context.repository,
        &execution.authorized_action,
        "governance.audit_legal_hold.upsert",
        "audit_legal_hold",
        Some(&event_id.to_string()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match context.repository.upsert_audit_legal_hold_idempotent(
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
    }
}

fn audit_retention_hold_response(
    context: &AuditRetentionResponseContext<'_>,
    event_id: &str,
) -> tiny_http::ResponseBox {
    if !context.captured.method.eq_ignore_ascii_case("DELETE") {
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
    let execution =
        match super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution(
            context.captured,
            context.path,
            context.admin_auth,
            context.base_action,
            ControlPlaneOperation::AuditLegalHoldDelete,
        ) {
            Ok(execution) => execution,
            Err(response) => return response,
        };
    let audit = match control_plane_audit_command(
        context.repository,
        &execution.authorized_action,
        "governance.audit_legal_hold.delete",
        "audit_legal_hold",
        Some(&event_id.to_string()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match context.repository.delete_audit_legal_hold_idempotent(
        context.base_action.tenant.tenant_id,
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
    }
}

struct AuditRetentionPurgeRequest {
    approval_id: ApprovalId,
    event_ids: Vec<AuditEventId>,
    retention_policy: AuditRetentionPolicy,
    batch: AuditRetentionPurgeBatch,
    durable_store: DurableStoreKind,
}

fn audit_retention_purge_request(
    captured: &RuntimeProxyRequest,
    tenant_id: prodex_domain::TenantId,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<AuditRetentionPurgeRequest, tiny_http::ResponseBox> {
    let body = runtime_gateway_admin_json_body(captured)?;
    let approval_id = body
        .get("approval_id")
        .and_then(serde_json::Value::as_str)
        .map(|value| ApprovalId::new(value.to_string()))
        .and_then(Result::ok)
        .ok_or_else(invalid_request)?;
    let mut event_ids = audit_retention_purge_event_ids(body.get("audit_event_ids"))?;
    event_ids.sort_unstable();
    event_ids.dedup();
    let retention_days = body
        .get("retention_days")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(invalid_request)
        })
        .transpose()?;
    let retention_policy =
        AuditRetentionPolicy::new(retention_days).map_err(|_| invalid_request())?;
    let batch_limit = AuditRetentionBatchLimit::new(u16::try_from(event_ids.len()).ok())
        .map_err(|_| invalid_request())?;
    let scope = AuditQueryScope::tenant(TenantContext { tenant_id });
    let keys = event_ids
        .iter()
        .copied()
        .map(|event_id| AuditRetentionPurgeKey {
            tenant_id,
            event_id,
        });
    let batch =
        AuditRetentionPurgeBatch::new(scope, keys, batch_limit).map_err(|_| invalid_request())?;
    let durable_store = match shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return Err(storage_unavailable());
        }
    };
    Ok(AuditRetentionPurgeRequest {
        approval_id,
        event_ids,
        retention_policy,
        batch,
        durable_store,
    })
}

fn audit_retention_purge_event_ids(
    value: Option<&serde_json::Value>,
) -> Result<Vec<AuditEventId>, tiny_http::ResponseBox> {
    let Some(values) = value.and_then(serde_json::Value::as_array) else {
        return Err(invalid_request());
    };
    if values.is_empty() || values.len() > usize::from(AuditRetentionBatchLimit::MAX) {
        return Err(invalid_request());
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(invalid_request)
                .and_then(|value| AuditEventId::from_str(value).map_err(|_| invalid_request()))
        })
        .collect()
}

struct AuditRetentionPurgeAuthorizationContext<'a> {
    captured: &'a RuntimeProxyRequest,
    path: &'a str,
    shared: &'a RuntimeLocalRewriteProxyShared,
    admin_auth: &'a RuntimeGatewayAdminAuth,
    repository: &'a RuntimeGovernanceRepository<'a>,
    tenant_id: prodex_domain::TenantId,
    approval_id: &'a ApprovalId,
    durable_store: DurableStoreKind,
}

fn authorize_audit_retention_purge(
    context: &AuditRetentionPurgeAuthorizationContext<'_>,
) -> Result<(), tiny_http::ResponseBox> {
    let approval = audit_retention_active_break_glass_approval(
        context.repository,
        context.tenant_id,
        context.approval_id,
    )?;
    let Some(reason) = approval.scope.as_str().strip_prefix("audit_retention:") else {
        return Err(break_glass_denied());
    };
    let http = runtime_gateway_http_request_meta(context.captured, context.path);
    let Some(mut action) = runtime_gateway_admin_control_plane_action_for_operation(
        &http,
        context.admin_auth,
        ControlPlaneOperation::AuditRetentionPurge,
    ) else {
        return Err(invalid_request());
    };
    action.principal = Principal::new(
        action.principal.id,
        Some(context.tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    );
    action.resource.kind = ResourceKind::AuditLog;
    let authorization = BreakGlassAuthorization {
        reason: reason.to_string(),
        expires_at_unix_ms: approval.expires_at_unix_ms,
    };
    authorize_audit_retention_break_glass(
        context.shared,
        context.repository,
        context.tenant_id,
        action,
        authorization,
        context.durable_store,
    )
}

fn audit_retention_active_break_glass_approval(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
    approval_id: &ApprovalId,
) -> Result<ApprovalRecord, tiny_http::ResponseBox> {
    match repository.get_approval(tenant_id, approval_id) {
        Ok(approval)
            if approval.kind == ApprovalKind::BreakGlass
                && approval.state == prodex_domain::ApprovalState::Active =>
        {
            Ok(approval)
        }
        Ok(_) | Err(GovernanceRepositoryError::NotFound) => Err(break_glass_denied()),
        Err(error) => Err(repository_error(error)),
    }
}

fn authorize_audit_retention_break_glass(
    shared: &RuntimeLocalRewriteProxyShared,
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
    action: prodex_control_plane::ControlPlaneActionRequest,
    authorization: BreakGlassAuthorization,
    durable_store: DurableStoreKind,
) -> Result<(), tiny_http::ResponseBox> {
    let previous_digest = repository
        .latest_audit_digest(tenant_id)
        .map_err(repository_error)?;
    let preview =
        prodex_control_plane::decide_break_glass_action(action.clone(), authorization.clone());
    let preview_event = match &preview {
        ControlPlaneDecision::Authorized(plan) => &plan.audit_event,
        ControlPlaneDecision::Denied { audit_event, .. } => audit_event,
    };
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), preview_event);
    let plan = plan_application_break_glass_with_audit_storage(ApplicationBreakGlassAuditRequest {
        durable_store,
        action,
        authorization,
        previous_digest,
        event_digest,
    })
    .map_err(|_| storage_unavailable())?;
    let (authorized, audit_event) = match plan.decision {
        ControlPlaneDecision::Authorized(plan) => (true, plan.audit_event),
        ControlPlaneDecision::Denied { audit_event, .. } => (false, audit_event),
    };
    super::local_rewrite_governance_audit::persist_runtime_control_plane_audit_event(
        shared,
        audit_event,
    )
    .map_err(|_| storage_unavailable())?;
    if !authorized {
        return Err(break_glass_denied());
    }
    Ok(())
}

fn audit_retention_purge_response(
    context: &AuditRetentionResponseContext<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    let purge = match audit_retention_purge_request(
        context.captured,
        context.base_action.tenant.tenant_id,
        shared,
    ) {
        Ok(purge) => purge,
        Err(response) => return response,
    };
    if plan_application_audit_retention_purge(ApplicationAuditRetentionPurgeRequest {
        durable_store: purge.durable_store,
        purge: AuditRetentionPurgeCommand {
            storage_key: TenantStorageKey::tenant(context.base_action.tenant.tenant_id),
            batch: purge.batch,
        },
    })
    .is_err()
    {
        return invalid_request();
    }

    let execution =
        match super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution(
            context.captured,
            context.path,
            context.admin_auth,
            context.base_action,
            ControlPlaneOperation::AuditRetentionPurge,
        ) {
            Ok(execution) => execution,
            Err(response) => return response,
        };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let authorization_context = AuditRetentionPurgeAuthorizationContext {
        captured: context.captured,
        path: context.path,
        shared,
        admin_auth: context.admin_auth,
        repository: context.repository,
        tenant_id,
        approval_id: &purge.approval_id,
        durable_store: purge.durable_store,
    };
    if let Err(response) = authorize_audit_retention_purge(&authorization_context) {
        return response;
    }

    let now_unix_ms = execution.atomic_write.completed_at_unix_ms;
    let cutoff_unix_ms = now_unix_ms
        .saturating_sub(u64::from(purge.retention_policy.days()).saturating_mul(86_400_000));
    let audit = match control_plane_audit_command(
        context.repository,
        &execution.authorized_action,
        "governance.audit_retention.purge",
        "audit_log",
        Some(purge.approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match context.repository.purge_audit_events_idempotent(
        tenant_id,
        &purge.event_ids,
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
                "requested": purge.event_ids.len(),
                "purged": purged.len(),
                "protected_or_ineligible": purge.event_ids.len().saturating_sub(purged.len()),
                "audit_event_ids": purged,
                "retention_days": purge.retention_policy.days(),
                "approval_id": purge.approval_id.as_str(),
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
