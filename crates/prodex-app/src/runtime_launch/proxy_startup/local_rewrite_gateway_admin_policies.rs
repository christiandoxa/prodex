use prodex_application::{
    ApplicationExecutionApprovalService, ApplicationGovernanceLifecycleError,
    ApplicationGovernanceRepository,
};
use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode,
    ApprovalRecord, ApprovalScope, AuditAction, AuditEventId, AuditResource, CredentialScope,
    Principal, PrincipalKind, Role, compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteIdempotency, ApprovalVoteMutationOutcome,
    ApprovalVoteRequest, ApprovalVoteSnapshot, AuditOutboxWriteCommand, GovernanceActivationAction,
    GovernanceActivationRequest, GovernanceActivationResult, GovernanceArtifactKind,
    GovernanceAuditExportRecord, GovernanceAuditIntegrityHealth, GovernanceOutboxHealth,
    GovernanceRepositoryError, GovernanceRevisionSummary, GovernanceRevisionWriteCommand,
    GovernanceSnapshot, GovernanceStatus, GovernanceWriteOutcome, TenantStorageKey,
};
use prodex_storage_sqlite_runtime::GovernanceSqliteRepository;
use sha2::{Digest, Sha256};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_boundary::{
    runtime_gateway_admin_control_plane_action_for_operation, runtime_gateway_now_unix_ms,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution;
use super::local_rewrite_gateway_admin_response::{
    runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::*;

#[derive(Clone, Copy)]
enum RuntimeGovernanceResource {
    Policy,
    ClassificationRules,
    ProviderRegistry,
    RoutingScores,
}

impl RuntimeGovernanceResource {
    const ALL: [Self; 4] = [
        Self::Policy,
        Self::ClassificationRules,
        Self::ProviderRegistry,
        Self::RoutingScores,
    ];

    fn prefix_suffix(self) -> &'static str {
        match self {
            Self::Policy => "/policies",
            Self::ClassificationRules => "/classification-rules",
            Self::ProviderRegistry => "/provider-registries",
            Self::RoutingScores => "/routing-scores",
        }
    }

    fn kind(self) -> GovernanceArtifactKind {
        match self {
            Self::Policy => GovernanceArtifactKind::Policy,
            Self::ClassificationRules => GovernanceArtifactKind::ClassificationRules,
            Self::ProviderRegistry => GovernanceArtifactKind::ProviderRegistry,
            Self::RoutingScores => GovernanceArtifactKind::RoutingScores,
        }
    }

    fn approval_kind(self) -> ApprovalKind {
        match self {
            Self::Policy => ApprovalKind::PolicyRevision,
            Self::ClassificationRules => ApprovalKind::ClassificationRuleRevision,
            Self::ProviderRegistry => ApprovalKind::ProviderRegistryRevision,
            Self::RoutingScores => ApprovalKind::RoutingScoreRevision,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::ClassificationRules => "classification_rules",
            Self::ProviderRegistry => "provider_registry",
            Self::RoutingScores => "routing_scores",
        }
    }
}

pub(super) fn runtime_gateway_admin_policy_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_prefix: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Option<tiny_http::ResponseBox> {
    let outbox = format!("{admin_prefix}/governance/outbox");
    let audit_integrity = format!("{admin_prefix}/governance/audit/integrity");
    let audit_export = format!("{admin_prefix}/audit/exports");
    if path == audit_export {
        return Some(audit_export_response(captured, shared, base_action));
    }
    if path == audit_integrity {
        return Some(audit_integrity_response(captured, shared, base_action));
    }
    if path == outbox || path == format!("{outbox}/claim") {
        return Some(outbox_response(
            captured,
            path,
            &outbox,
            shared,
            admin_auth,
            base_action,
        ));
    }
    let execution_approvals = format!("{admin_prefix}/execution-approvals");
    if path == execution_approvals || path.starts_with(&(execution_approvals.clone() + "/")) {
        let repository = match repository(shared) {
            Ok(repository) => repository,
            Err(response) => return Some(response),
        };
        return Some(execution_approval_response(
            captured,
            path,
            &execution_approvals,
            admin_auth,
            base_action,
            &repository,
        ));
    }
    let (resource, resource_path) =
        RuntimeGovernanceResource::ALL
            .into_iter()
            .find_map(|resource| {
                let resource_path = format!("{admin_prefix}{}", resource.prefix_suffix());
                (path == resource_path || path.starts_with(&(resource_path.clone() + "/")))
                    .then_some((resource, resource_path))
            })?;
    let suffix = path.strip_prefix(&(resource_path.clone() + "/"));
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return Some(response),
    };
    let tenant_id = base_action.tenant.tenant_id;
    let method = captured.method.to_ascii_uppercase();
    let segments = suffix
        .map(|value| {
            value
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(match (method.as_str(), segments.as_slice()) {
        ("POST", ["validate"]) => validate_response(captured, shared, tenant_id, resource),
        ("POST", []) => create_response(
            captured,
            path,
            shared,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        ("GET", []) => match repository.list_revisions(tenant_id, resource.kind()) {
            Ok(revisions) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": format!("governance.{}_revision.list", resource.label()),
                    "data": revisions
                        .into_iter()
                        .map(|revision| revision_json(revision, resource))
                        .collect::<Vec<_>>()
                }),
            ),
            Err(error) => repository_error(error),
        },
        ("GET", ["status"]) => status_response(&repository, tenant_id, resource),
        ("GET", [revision_id]) => {
            match repository.get_revision(tenant_id, resource.kind(), revision_id) {
                Ok(revision) => {
                    runtime_gateway_admin_json_response(200, revision_json(revision, resource))
                }
                Err(error) => repository_error(error),
            }
        }
        ("POST", [revision_id, "submit"]) => submit_response(
            captured,
            path,
            revision_id,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        ("POST", [revision_id, "approvals", approval_id, "votes"]) => vote_response(
            captured,
            path,
            revision_id,
            approval_id,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        ("POST", [revision_id, action @ ("activate" | "rollback")]) => activation_response(
            captured,
            path,
            revision_id,
            action,
            resource,
            shared,
            admin_auth,
            base_action,
            &repository,
        ),
        ("GET" | "POST", _) => build_runtime_proxy_json_error_response(
            404,
            "governance_policy_not_found",
            "governance resource was not found",
        ),
        _ => build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance route",
        ),
    })
}

pub(super) enum RuntimeGovernanceRepository<'a> {
    Sqlite(GovernanceSqliteRepository),
    Postgres {
        repository: &'a prodex_storage_postgres_runtime::PostgresRepository,
        runtime: tokio::runtime::Handle,
    },
}

impl RuntimeGovernanceRepository<'_> {
    fn write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.write_revision(command, audit),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_write_revision(command, audit)),
        }
    }

    pub(super) fn create_approval(
        &self,
        approval: ApprovalRecord,
        audit: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.create_approval(approval, audit),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_create_approval(approval, audit)),
        }
    }

    pub(super) fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.transition_approval(request, action),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_transition_approval(request, action)),
        }
    }

    fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.transition_approval_idempotent(request, action, idempotency)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_transition_approval_idempotent(
                request,
                action,
                idempotency,
            )),
        }
    }

    fn list_revisions(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_revisions(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_revisions(tenant_id, kind)),
        }
    }

    fn get_revision(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.get_revision(tenant_id, kind, revision_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_get_revision(tenant_id, kind, revision_id)),
        }
    }

    pub(super) fn get_approval(
        &self,
        tenant_id: prodex_domain::TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.get_approval(tenant_id, approval_id),
            Self::Postgres {
                repository,
                runtime,
            } => {
                runtime.block_on(repository.governance_get_approval(tenant_id, approval_id.clone()))
            }
        }
    }

    fn list_execution_approvals(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_execution_approvals(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_execution_approvals(tenant_id)),
        }
    }

    fn status(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.status(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_status(tenant_id, kind)),
        }
    }

    fn load_snapshot(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: GovernanceArtifactKind,
        validate_artifact: impl FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.load_snapshot(tenant_id, kind, validate_artifact)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_load_snapshot(
                tenant_id,
                kind,
                validate_artifact,
            )),
        }
    }

    pub(super) fn latest_audit_digest(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Option<prodex_domain::AuditDigest>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.latest_audit_digest(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_latest_audit_digest(tenant_id)),
        }
    }

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: impl FnOnce(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.activate_revision(request, validate_artifact),
            Self::Postgres {
                repository,
                runtime,
            } => runtime
                .block_on(repository.governance_activate_revision(request, validate_artifact)),
        }
    }

    fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.append_audit_outbox(command),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_append_audit_outbox(command)),
        }
    }

    fn outbox_health(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.outbox_health(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_outbox_health(tenant_id)),
        }
    }

    fn audit_integrity_health(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.audit_integrity_health(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_audit_integrity_health(tenant_id)),
        }
    }

    fn export_audit(
        &self,
        tenant_id: prodex_domain::TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.governance_export_audit(tenant_id, limit),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_export_audit(tenant_id, limit)),
        }
    }
}

impl ApplicationGovernanceRepository for RuntimeGovernanceRepository<'_> {
    fn write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::write_revision(self, command, audit_outbox)
    }

    fn create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::create_approval(self, approval, audit_outbox)
    }

    fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::transition_approval(self, request, action)
    }

    fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::transition_approval_idempotent(
            self,
            request,
            action,
            idempotency,
        )
    }

    fn get_approval(
        &self,
        tenant_id: prodex_domain::TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::get_approval(self, tenant_id, approval_id)
    }

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        RuntimeGovernanceRepository::activate_revision(self, request, validate_artifact)
    }

    fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        RuntimeGovernanceRepository::append_audit_outbox(self, command)
    }
}

pub(super) fn repository(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGovernanceRepository<'_>, tiny_http::ResponseBox> {
    runtime_governance_repository(shared).map_err(|_| storage_unavailable())
}

pub(super) fn runtime_governance_repository(
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGovernanceRepository<'_>, GovernanceRepositoryError> {
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { path } => GovernanceSqliteRepository::open(path)
            .map(RuntimeGovernanceRepository::Sqlite)
            .map_err(|_| GovernanceRepositoryError::Database),
        RuntimeGatewayStateStore::Postgres { .. } => shared
            .gateway_postgres_repository
            .as_ref()
            .map(|repository| RuntimeGovernanceRepository::Postgres {
                repository,
                runtime: shared.runtime_shared.async_runtime.handle().clone(),
            })
            .ok_or(GovernanceRepositoryError::Database),
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            Err(GovernanceRepositoryError::Unsupported)
        }
    }
}

fn storage_unavailable() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        503,
        "governance_policy_storage_unavailable",
        "policy governance storage is temporarily unavailable",
    )
}

fn governance_artifact_is_valid(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
    resource: RuntimeGovernanceResource,
    expected_revision_id: Option<&str>,
    artifact: &[u8],
) -> bool {
    match resource {
        RuntimeGovernanceResource::Policy => crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                artifact,
                shared.runtime_shared.runtime_config.governance.mode,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id.is_none_or(|expected| {
                    snapshot.application.policy.revision().to_string() == expected
                })
            }),
        RuntimeGovernanceResource::ClassificationRules => super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                tenant_id,
                artifact,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id.is_none_or(|expected| {
                    snapshot.classification_rules().revision().as_str() == expected
                })
            }),
        RuntimeGovernanceResource::ProviderRegistry => super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact(
                artifact,
                &shared.provider,
                shared.provider_credential.as_ref(),
            )
            .is_ok_and(|snapshot| {
                expected_revision_id
                    .is_none_or(|expected| snapshot.revision().to_string() == expected)
            }),
        RuntimeGovernanceResource::RoutingScores => super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                artifact,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id
                    .is_none_or(|expected| snapshot.revision.to_string() == expected)
            }),
    }
}

fn validate_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
    resource: RuntimeGovernanceResource,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(artifact) = body.get("artifact") else {
        return invalid_request();
    };
    let Ok(bytes) = serde_json::to_vec(artifact) else {
        return invalid_request();
    };
    if bytes.is_empty() || bytes.len() > prodex_storage::MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES {
        return invalid_request();
    }
    if !governance_artifact_is_valid(shared, tenant_id, resource, None, &bytes) {
        return invalid_request();
    }
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "object": format!("governance.{}_validation", resource.label()),
            "valid": true,
            "fingerprint": artifact_fingerprint(&bytes),
        }),
    )
}

fn create_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(revision_id) = body.get("revision_id").and_then(serde_json::Value::as_str) else {
        return invalid_request();
    };
    let Some(artifact) = body.get("artifact") else {
        return invalid_request();
    };
    let Ok(compiled_artifact) = serde_json::to_vec(artifact) else {
        return invalid_request();
    };
    if !governance_artifact_is_valid(
        shared,
        base_action.tenant.tenant_id,
        resource,
        Some(revision_id),
        &compiled_artifact,
    ) {
        return invalid_request();
    }
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let fingerprint = artifact_fingerprint(&compiled_artifact);
    let Ok(fingerprint_value) = ApprovalFingerprint::new(fingerprint.clone()) else {
        return invalid_request();
    };
    let command = GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: resource.kind(),
        revision_id: revision_id.to_string(),
        fingerprint: fingerprint_value,
        compiled_artifact,
        created_by: execution.authorized_action.audit_event.principal_id,
        created_at_unix_ms: execution.atomic_write.completed_at_unix_ms,
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &format!("governance.{}.revision.write", resource.label()),
        Some(revision_id),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match repository.write_revision(command, audit) {
        Ok(outcome) => runtime_gateway_admin_json_response(
            if outcome == GovernanceWriteOutcome::Applied {
                201
            } else {
                200
            },
            serde_json::json!({
                "object": format!("governance.{}_revision", resource.label()),
                "revision_id": revision_id,
                "fingerprint": fingerprint,
                "state": "draft",
                "replayed": outcome == GovernanceWriteOutcome::Replayed,
            }),
        ),
        Err(error) => repository_error(error),
    }
}

fn submit_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let revision = match repository.get_revision(tenant_id, resource.kind(), revision_id) {
        Ok(revision) => revision,
        Err(error) => return repository_error(error),
    };
    let approval_id = body
        .get("approval_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let required_quorum = body
        .get("required_quorum")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(1);
    let expires_at = body
        .get("expires_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| runtime_gateway_now_unix_ms().saturating_add(86_400_000));
    let Ok(approval) = ApprovalRecord::pending(
        match ApprovalId::new(approval_id.clone()) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        tenant_id,
        resource.approval_kind(),
        match ApprovalScope::new(
            body.get("scope")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}/{revision_id}/{approval_id}", resource.label())),
        ) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        match ApprovalFingerprint::new(revision.fingerprint) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        execution.authorized_action.audit_event.principal_id,
        required_quorum,
        expires_at,
    ) else {
        return invalid_request();
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &format!("governance.{}.approval.create", resource.label()),
        Some(&approval_id),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match repository.create_approval(approval, audit) {
        Ok(outcome) => runtime_gateway_admin_json_response(
            if outcome == GovernanceWriteOutcome::Applied {
                201
            } else {
                200
            },
            serde_json::json!({
                "object": format!("governance.{}_approval", resource.label()),
                "approval_id": approval_id,
                "revision_id": revision_id,
                "state": "pending_approval",
                "version": 1,
                "replayed": outcome == GovernanceWriteOutcome::Replayed,
            }),
        ),
        Err(error) => repository_error(error),
    }
}

fn execution_approval_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    base_path: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let method = captured.method.to_ascii_uppercase();
    let segments = path
        .strip_prefix(&(base_path.to_string() + "/"))
        .map(|suffix| {
            suffix
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tenant_id = base_action.tenant.tenant_id;
    match (method.as_str(), segments.as_slice()) {
        ("GET", []) => match repository.list_execution_approvals(tenant_id) {
            Ok(approvals) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "governance.execution_approval.list",
                    "data": approvals.into_iter().map(execution_approval_json).collect::<Vec<_>>(),
                }),
            ),
            Err(error) => repository_error(error),
        },
        ("GET", [approval_id]) => {
            let approval_id = match ApprovalId::new((*approval_id).to_string()) {
                Ok(approval_id) => approval_id,
                Err(_) => return invalid_request(),
            };
            match repository.get_approval(tenant_id, &approval_id) {
                Ok(approval) if approval.kind == ApprovalKind::Execution => {
                    runtime_gateway_admin_json_response(200, execution_approval_json(approval))
                }
                Ok(_) => repository_error(GovernanceRepositoryError::NotFound),
                Err(error) => repository_error(error),
            }
        }
        ("POST", [approval_id, "votes"]) => {
            let body = match runtime_gateway_admin_json_body(captured) {
                Ok(body) => body,
                Err(response) => return response,
            };
            let Some(expected_version) = body
                .get("expected_version")
                .and_then(serde_json::Value::as_u64)
            else {
                return invalid_request();
            };
            let action = match body.get("decision").and_then(serde_json::Value::as_str) {
                Some("approve") => ApprovalAction::Approve,
                Some("reject") => ApprovalAction::Reject,
                Some("cancel") => ApprovalAction::Cancel,
                _ => return invalid_request(),
            };
            let approval_id = match ApprovalId::new((*approval_id).to_string()) {
                Ok(approval_id) => approval_id,
                Err(_) => return invalid_request(),
            };
            let execution = match execution(captured, path, admin_auth, base_action) {
                Ok(execution) => execution,
                Err(response) => return response,
            };
            let audit = match execution_approval_audit_command(
                repository,
                &execution.authorized_action,
                &approval_id,
                match action {
                    ApprovalAction::Approve => "governance.execution_approval.approve",
                    ApprovalAction::Reject => "governance.execution_approval.reject",
                    ApprovalAction::Cancel => "governance.execution_approval.cancel",
                    _ => unreachable!(),
                },
            ) {
                Ok(audit) => audit,
                Err(error) => return repository_error(error),
            };
            let reason = match action {
                ApprovalAction::Reject => {
                    Some(ApprovalReasonCode::new("approval.rejected").unwrap())
                }
                ApprovalAction::Cancel => {
                    Some(ApprovalReasonCode::new("approval.cancelled").unwrap())
                }
                _ => None,
            };
            match ApplicationExecutionApprovalService::new(repository).review_idempotent(
                ApprovalVoteRequest {
                    tenant_id,
                    approval_id: approval_id.clone(),
                    actor: actor(&execution.authorized_action),
                    expected_version,
                    now_unix_ms: execution.atomic_write.completed_at_unix_ms,
                    reason,
                    audit_outbox: audit,
                },
                action,
                ApprovalVoteIdempotency {
                    operation: execution.atomic_write.operation.clone(),
                    started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
                },
            ) {
                Ok(ApprovalVoteMutationOutcome::Applied(approval)) => {
                    runtime_gateway_admin_json_response(200, execution_approval_json(approval))
                }
                Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)) => {
                    runtime_gateway_admin_json_response(
                        200,
                        execution_approval_snapshot_json(&approval_id, snapshot),
                    )
                }
                Err(ApplicationGovernanceLifecycleError::Repository(error)) => {
                    repository_error(error)
                }
                Err(_) => repository_error(GovernanceRepositoryError::InvalidInput),
            }
        }
        ("GET" | "POST", _) => build_runtime_proxy_json_error_response(
            404,
            "execution_approval_not_found",
            "execution approval was not found",
        ),
        _ => build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance route",
        ),
    }
}

fn execution_approval_json(approval: ApprovalRecord) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.execution_approval",
        "approval_id": approval.id.as_str(),
        "state": approval_state(approval.state),
        "version": approval.version,
        "required_quorum": approval.effective_required_quorum(),
        "vote_count": approval.votes.len(),
        "expires_at_unix_ms": approval.expires_at_unix_ms,
        "activated_at_unix_ms": approval.activated_at_unix_ms,
    })
}

fn execution_approval_snapshot_json(
    approval_id: &ApprovalId,
    snapshot: ApprovalVoteSnapshot,
) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.execution_approval",
        "approval_id": approval_id.as_str(),
        "state": approval_state(snapshot.state),
        "version": snapshot.version,
        "required_quorum": snapshot.required_quorum,
        "vote_count": snapshot.vote_count,
        "expires_at_unix_ms": snapshot.expires_at_unix_ms,
        "activated_at_unix_ms": snapshot.activated_at_unix_ms,
    })
}

fn execution_approval_audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    approval_id: &ApprovalId,
    audit_action: &str,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action = AuditAction::new(audit_action);
    event.resource = AuditResource::new(
        "execution_approval",
        Some(approval_id.as_str().to_string()),
        Some(event.tenant_id),
    );
    let previous_digest = repository.latest_audit_digest(event.tenant_id)?;
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    Ok(AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(event.tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn vote_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    approval_id: &str,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let decision = match body.get("decision").and_then(serde_json::Value::as_str) {
        Some("approve") => ApprovalAction::Approve,
        Some("reject") => ApprovalAction::Reject,
        _ => return invalid_request(),
    };
    let Some(expected_version) = body
        .get("expected_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return invalid_request();
    };
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(value) => value,
        Err(_) => return invalid_request(),
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let revision = match repository.get_revision(tenant_id, resource.kind(), revision_id) {
        Ok(revision) => revision,
        Err(error) => return repository_error(error),
    };
    let approval = match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) => approval,
        Err(error) => return repository_error(error),
    };
    if approval.kind != resource.approval_kind()
        || approval.fingerprint.as_str() != revision.fingerprint
    {
        return repository_error(GovernanceRepositoryError::NotFound);
    }
    let audit_action = match decision {
        ApprovalAction::Approve => {
            format!("governance.{}.approval.approve", resource.label())
        }
        _ => format!("governance.{}.approval.reject", resource.label()),
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &audit_action,
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let actor = actor(&execution.authorized_action);
    match repository.transition_approval(
        ApprovalVoteRequest {
            tenant_id,
            approval_id,
            actor,
            expected_version,
            now_unix_ms: execution.atomic_write.completed_at_unix_ms,
            reason: None,
            audit_outbox: audit,
        },
        decision,
    ) {
        Ok(approval) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": format!("governance.{}_approval", resource.label()),
                "state": approval_state(approval.state),
                "version": approval.version,
                "vote_count": approval.votes.len(),
            }),
        ),
        Err(error) => repository_error(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn activation_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    action: &str,
    resource: RuntimeGovernanceResource,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(approval_id) = body.get("approval_id").and_then(serde_json::Value::as_str) else {
        return invalid_request();
    };
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(value) => value,
        Err(_) => return invalid_request(),
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let Some(entity_tag) = execution.entity_tag.as_ref() else {
        return build_runtime_proxy_json_error_response(
            428,
            "control_plane_if_match_required",
            "If-Match is required for governance activation",
        );
    };
    let activation_action = if action == "activate" {
        GovernanceActivationAction::Activate
    } else {
        GovernanceActivationAction::Rollback
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let audit_action = if action == "activate" {
        format!("governance.{}.revision.activate", resource.label())
    } else {
        format!("governance.{}.revision.rollback", resource.label())
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &audit_action,
        Some(revision_id),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let expected_etag = (entity_tag.as_str() != "*").then(|| entity_tag.as_str().to_string());
    match repository.activate_revision(
        GovernanceActivationRequest {
            tenant_id,
            kind: resource.kind(),
            revision_id: revision_id.to_string(),
            approval_id,
            actor: actor(&execution.authorized_action),
            action: activation_action,
            expected_etag,
            idempotency_key: execution.atomic_write.operation.key.clone(),
            request_fingerprint: execution.atomic_write.operation.request_fingerprint.clone(),
            audit_outbox: audit,
            activated_at_unix_ms: execution.atomic_write.completed_at_unix_ms,
        },
        |artifact| {
            governance_artifact_is_valid(shared, tenant_id, resource, Some(revision_id), artifact)
        },
    ) {
        Ok(result) => {
            let snapshot = match repository.load_snapshot(tenant_id, resource.kind(), |artifact| {
                governance_artifact_is_valid(
                    shared,
                    tenant_id,
                    resource,
                    Some(revision_id),
                    artifact,
                )
            }) {
                Ok(snapshot) => snapshot,
                Err(error) => return repository_error(error),
            };
            if shared
                .swap_committed_governance_artifact_kind(
                    tenant_id,
                    resource.kind(),
                    &snapshot.compiled_artifact,
                )
                .is_err()
            {
                return repository_error(GovernanceRepositoryError::SnapshotUnavailable);
            }
            json_response_with_etag(
                200,
                serde_json::json!({
                    "object": format!("governance.{}_activation", resource.label()),
                    "revision_id": result.revision_id,
                    "last_known_good_revision_id": result.last_known_good_revision_id,
                    "etag": result.etag,
                    "replayed": result.outcome == GovernanceWriteOutcome::Replayed,
                }),
                &result.etag,
            )
        }
        Err(error) => repository_error(error),
    }
}

fn status_response(
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

fn outbox_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    outbox: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> tiny_http::ResponseBox {
    if path == format!("{outbox}/claim") {
        return if captured.method.eq_ignore_ascii_case("POST") {
            if let Err(response) = execution(captured, path, admin_auth, base_action) {
                return response;
            }
            build_runtime_proxy_json_error_response(
                501,
                "governance_outbox_claim_unavailable",
                "SIEM outbox claiming requires a lease-capable worker backend",
            )
        } else {
            build_runtime_proxy_json_error_response(
                405,
                "control_plane_method_not_allowed",
                "HTTP method is not allowed for this governance outbox route",
            )
        };
    }
    if !captured.method.eq_ignore_ascii_case("GET") {
        return build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance outbox route",
        );
    }
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    match repository.outbox_health(base_action.tenant.tenant_id) {
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

fn audit_integrity_response(
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

fn audit_export_response(
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
        100
    } else {
        match runtime_gateway_admin_json_body(captured)
            .ok()
            .and_then(|body| body.get("limit").and_then(serde_json::Value::as_u64))
            .and_then(|limit| u16::try_from(limit).ok())
            .filter(|limit| (1..=1_000).contains(limit))
        {
            Some(limit) => limit,
            None => {
                return build_runtime_proxy_json_error_response(
                    400,
                    "governance_audit_export_invalid",
                    "audit export limit must be between 1 and 1000",
                );
            }
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
    let mut audit_result = Err(GovernanceRepositoryError::AuditChainConflict);
    for _ in 0..3 {
        let mut event = base_action.audit_event.clone();
        event.action = AuditAction::new("control_plane.audit.export");
        event.resource = AuditResource::new(
            "governance_audit_export",
            Some(format!("limit:{limit}")),
            Some(tenant_id),
        );
        let previous_digest = match repository.latest_audit_digest(tenant_id) {
            Ok(digest) => digest,
            Err(error) => return repository_error(error),
        };
        let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
        let command = AuditOutboxWriteCommand {
            outbox_event_id: AuditEventId::new(),
            audit: AppendOnlyAuditCommand {
                storage_key: TenantStorageKey::tenant(tenant_id),
                event,
                previous_digest,
                event_digest,
            },
        };
        audit_result = repository.append_audit_outbox(command);
        if !matches!(
            audit_result,
            Err(GovernanceRepositoryError::AuditChainConflict)
        ) {
            break;
        }
    }
    if let Err(error) = audit_result {
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

fn execution(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Result<
    super::local_rewrite_gateway_admin_execution::RuntimeGatewayAdminMutationExecution,
    tiny_http::ResponseBox,
> {
    let http = runtime_gateway_http_request_meta(captured, path);
    runtime_gateway_admin_control_plane_action_for_operation(
        &http,
        admin_auth,
        ControlPlaneOperation::PolicyPublish,
    )
    .ok_or_else(invalid_request)?;
    runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        ControlPlaneOperation::PolicyPublish,
    )
}

fn audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    resource: RuntimeGovernanceResource,
    audit_action: &str,
    resource_id: Option<&str>,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action = AuditAction::new(audit_action);
    event.resource = AuditResource::new(
        format!("governance_{}_revision", resource.label()),
        resource_id.map(str::to_string),
        Some(event.tenant_id),
    );
    let previous_digest = repository.latest_audit_digest(event.tenant_id)?;
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    Ok(AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(event.tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    })
}

fn actor(action: &ControlPlaneActionPlan) -> Principal {
    Principal::new(
        action.audit_event.principal_id,
        Some(action.tenant.tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

fn artifact_fingerprint(artifact: &[u8]) -> String {
    let digest = Sha256::digest(artifact);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn revision_json(
    revision: GovernanceRevisionSummary,
    resource: RuntimeGovernanceResource,
) -> serde_json::Value {
    serde_json::json!({
        "object": format!("governance.{}_revision", resource.label()),
        "revision_id": revision.revision_id,
        "fingerprint": revision.fingerprint,
        "state": revision.lifecycle_state,
        "created_at_unix_ms": revision.created_at_unix_ms,
    })
}

fn approval_state(state: prodex_domain::ApprovalState) -> &'static str {
    match state {
        prodex_domain::ApprovalState::Draft => "draft",
        prodex_domain::ApprovalState::PendingApproval => "pending_approval",
        prodex_domain::ApprovalState::Approved => "approved",
        prodex_domain::ApprovalState::Rejected => "rejected",
        prodex_domain::ApprovalState::Expired => "expired",
        prodex_domain::ApprovalState::Cancelled => "cancelled",
        prodex_domain::ApprovalState::Active => "active",
        prodex_domain::ApprovalState::Superseded => "superseded",
        prodex_domain::ApprovalState::RolledBack => "rolled_back",
    }
}

fn repository_error(error: GovernanceRepositoryError) -> tiny_http::ResponseBox {
    let (status, code, message) = match error {
        GovernanceRepositoryError::InvalidInput => (
            400,
            "governance_policy_invalid",
            "policy governance request is invalid",
        ),
        GovernanceRepositoryError::TenantMismatch => (
            403,
            "governance_policy_forbidden",
            "policy governance request is forbidden",
        ),
        GovernanceRepositoryError::NotFound => (
            404,
            "governance_policy_not_found",
            "policy governance resource was not found",
        ),
        GovernanceRepositoryError::EtagMismatch => (
            412,
            "governance_policy_precondition_failed",
            "policy governance precondition failed",
        ),
        GovernanceRepositoryError::ApprovalSelfAction => (
            403,
            "governance_policy_self_approval_forbidden",
            "policy maker cannot approve this revision",
        ),
        GovernanceRepositoryError::StaleVersion => (
            409,
            "governance_policy_version_stale",
            "policy approval version is stale",
        ),
        GovernanceRepositoryError::Conflict
        | GovernanceRepositoryError::InvalidTransition
        | GovernanceRepositoryError::ApprovalRequired => (
            409,
            "governance_policy_conflict",
            "policy governance state conflicts with this request",
        ),
        GovernanceRepositoryError::SnapshotUnavailable => (
            422,
            "governance_policy_snapshot_invalid",
            "policy revision could not be verified",
        ),
        GovernanceRepositoryError::Unsupported => (
            501,
            "governance_policy_operation_unsupported",
            "policy governance operation is not supported by this backend",
        ),
        GovernanceRepositoryError::Database | GovernanceRepositoryError::AuditChainConflict => (
            503,
            "governance_policy_storage_unavailable",
            "policy governance storage is temporarily unavailable",
        ),
    };
    build_runtime_proxy_json_error_response(status, code, message)
}

fn invalid_request() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        400,
        "governance_policy_invalid",
        "policy governance request is invalid",
    )
}

fn json_response_with_etag(
    status: u16,
    value: serde_json::Value,
    etag: &str,
) -> tiny_http::ResponseBox {
    let body = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![
            (
                "content-type".to_string(),
                b"application/json; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
            ("etag".to_string(), etag.as_bytes().to_vec()),
        ],
        body: body.into(),
    })
}
