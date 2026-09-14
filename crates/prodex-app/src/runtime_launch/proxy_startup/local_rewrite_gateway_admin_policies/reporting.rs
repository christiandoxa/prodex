use super::*;

pub(super) fn status_response(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
    resource: RuntimeGovernanceResource,
) -> tiny_http::ResponseBox {
    match repository.status(tenant_id, resource.kind()) {
        Ok(status) => {
            let value = serde_json::json!({
                "object": format!("governance.{}_status", resource.label()),
                "active_revision_id": status.active_revision_id,
                "last_known_good_revision_id": status.last_known_good_revision_id,
                "etag": status.etag,
            });
            match status.etag.as_deref() {
                Some(etag) => json_response_with_etag(200, value, etag),
                None => runtime_gateway_admin_json_response(200, value),
            }
        }
        Err(error) => repository_error(error),
    }
}

pub(super) fn outbox_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    outbox: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if path == format!("{outbox}/claim") {
        return outbox_claim_response(captured, path, shared, admin_auth, base_action);
    }
    if !captured.method.eq_ignore_ascii_case("GET") {
        return build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance outbox route",
        );
    }
    outbox_health_response(shared, base_action.tenant.tenant_id)
}

fn outbox_claim_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if !captured.method.eq_ignore_ascii_case("POST") {
        return build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance outbox route",
        );
    }
    if let Err(response) = execution(captured, path, admin_auth, base_action) {
        return response;
    }
    if shared.gateway_observability.siem_worker.is_none() {
        return build_runtime_proxy_json_error_response(
            503,
            "governance_outbox_exporter_unavailable",
            "SIEM outbox exporter is not configured",
        );
    }
    outbox_claim_store_response(shared, base_action.tenant.tenant_id)
}

fn outbox_claim_store_response(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
) -> tiny_http::ResponseBox {
    let worker = shared
        .gateway_observability
        .siem_worker
        .as_ref()
        .expect("checked before store dispatch");
    let now_unix_ms = runtime_gateway_now_unix_ms();
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Postgres { .. } => {
            let Some(repository) = shared.gateway_postgres_repository.as_ref() else {
                return repository_error(GovernanceRepositoryError::Database);
            };
            match worker.run_once_postgres(
                repository,
                shared.runtime_shared.async_runtime.handle(),
                &[tenant_id],
                now_unix_ms,
            ) {
                Ok(()) => runtime_gateway_admin_json_response(
                    200,
                    serde_json::json!({
                        "object": "governance.siem_outbox_claim",
                        "status": "completed",
                    }),
                ),
                Err(error) => repository_error(error),
            }
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            let repository =
                match prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path) {
                    Ok(repository) => repository,
                    Err(error) => return repository_error(error),
                };
            match worker.run_once(&repository, now_unix_ms) {
                Ok(report) => runtime_gateway_admin_json_response(
                    200,
                    serde_json::json!({
                        "object": "governance.siem_outbox_claim",
                        "status": "completed",
                        "selected": report.selected,
                        "delivered": report.delivered,
                        "retried": report.retried,
                        "dead_lettered": report.dead_lettered,
                    }),
                ),
                Err(error) => repository_error(error),
            }
        }
        _ => repository_error(GovernanceRepositoryError::Unsupported),
    }
}

fn outbox_health_response(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
) -> tiny_http::ResponseBox {
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    match repository.outbox_health(tenant_id) {
        Ok(health) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "governance.siem_outbox_health",
                "pending": health.pending,
                "dead_lettered": health.dead_lettered,
                "oldest_pending_at_unix_ms": health.oldest_pending_at_unix_ms,
            }),
        ),
        Err(error) => repository_error(error),
    }
}

pub(super) fn audit_integrity_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if !captured.method.eq_ignore_ascii_case("GET") {
        return build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance audit route",
        );
    }
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    match repository.audit_integrity_health(base_action.tenant.tenant_id) {
        Ok(health) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "event_count": health.event_count,
                "chain_head_count": health.chain_head_count,
                "chain_valid": health.chain_valid,
            }),
        ),
        Err(error) => repository_error(error),
    }
}

pub(super) fn audit_export_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if !captured.method.eq_ignore_ascii_case("POST") {
        return build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this audit export route",
        );
    }
    let limit = if captured.body.is_empty() {
        match gateway_admin_audit_export_limit(None) {
            Ok(limit) => limit,
            Err(response) => return response,
        }
    } else {
        let body = match runtime_gateway_admin_json_body(captured) {
            Ok(body) => body,
            Err(_) => {
                return build_runtime_proxy_json_error_response(
                    400,
                    "governance_audit_export_invalid",
                    "audit export limit must be between 1 and 1000",
                );
            }
        };
        let Some(requested_limit) = body.get("limit").and_then(serde_json::Value::as_u64) else {
            return build_runtime_proxy_json_error_response(
                400,
                "governance_audit_export_invalid",
                "audit export limit must be between 1 and 1000",
            );
        };
        match gateway_admin_audit_export_limit(Some(requested_limit)) {
            Ok(limit) => limit,
            Err(response) => return response,
        }
    };
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let tenant_id = base_action.tenant.tenant_id;
    let records = match repository.export_audit(tenant_id, limit) {
        Ok(records) => records,
        Err(error) => return repository_error(error),
    };
    if let Err(error) = append_control_plane_audit_command(
        &repository,
        base_action,
        "control_plane.audit.export",
        "governance_audit_export",
        Some(&format!("limit:{limit}")),
    ) {
        return repository_error(error);
    }
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "object": "governance.audit_export",
            "data": records.into_iter().map(|record| serde_json::json!({
                "audit_event_id": record.audit_event_id,
                "occurred_at_unix_ms": record.occurred_at_unix_ms,
                "principal_id": record.principal_id,
                "action": record.action,
                "resource_kind": record.resource_kind,
                "resource_id": record.resource_id,
                "outcome": record.outcome,
                "reason_code": record.reason_code,
                "previous_digest": record.previous_digest,
                "event_digest": record.event_digest,
            })).collect::<Vec<_>>()
        }),
    )
}

pub(super) fn gateway_admin_audit_export_limit(
    requested_limit: Option<u64>,
) -> Result<u16, tiny_http::ResponseBox> {
    #[cfg(feature = "mojo-core")]
    {
        prodex_mojo_core::policy::plan_gateway_admin_limit(requested_limit).map_err(|_| {
            build_runtime_proxy_json_error_response(
                400,
                "governance_audit_export_invalid",
                "audit export limit must be between 1 and 1000",
            )
        })
    }

    #[cfg(not(feature = "mojo-core"))]
    {
        let limit = requested_limit.unwrap_or(100);
        if (1..=1_000).contains(&limit) {
            u16::try_from(limit).map_err(|_| {
                build_runtime_proxy_json_error_response(
                    400,
                    "governance_audit_export_invalid",
                    "audit export limit must be between 1 and 1000",
                )
            })
        } else {
            Err(build_runtime_proxy_json_error_response(
                400,
                "governance_audit_export_invalid",
                "audit export limit must be between 1 and 1000",
            ))
        }
    }
}
