use prodex_application::{
    ApplicationAuditRetentionPurgeRequest, ApplicationBreakGlassAuditRequest,
    ApplicationExecutionApprovalService, ApplicationGovernanceLifecycleError,
    ApplicationGovernanceRepository, plan_application_audit_retention_purge,
    plan_application_break_glass_with_audit_storage,
};
use prodex_control_plane::{
    BreakGlassAuthorization, ControlPlaneActionPlan, ControlPlaneDecision, ControlPlaneOperation,
};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode,
    ApprovalRecord, ApprovalScope, AuditAction, AuditEventId, AuditQueryScope, AuditReasonCode,
    AuditResource, AuditRetentionBatchLimit, AuditRetentionHold, AuditRetentionPolicy,
    AuditRetentionPurgeBatch, AuditRetentionPurgeKey, AuditTimestamp, CredentialScope, Principal,
    PrincipalKind, ResourceKind, Role, TenantContext, compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteIdempotency, ApprovalVoteMutationOutcome,
    ApprovalVoteRequest, ApprovalVoteSnapshot, AuditOutboxWriteCommand, AuditRetentionPurgeCommand,
    DurableStoreKind, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceArtifactKind, GovernanceAuditExportRecord,
    GovernanceAuditIntegrityHealth, GovernanceOutboxHealth, GovernanceRepositoryError,
    GovernanceRevisionSummary, GovernanceRevisionWriteCommand, GovernanceSnapshot,
    GovernanceStatus, GovernanceWriteOutcome, TenantStorageKey,
};
use prodex_storage_sqlite_runtime::GovernanceSqliteRepository;
use sha2::{Digest, Sha256};
use std::str::FromStr;

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
    let audit_retention = format!("{admin_prefix}/audit/retention");
    if path == format!("{audit_retention}/holds")
        || path.starts_with(&(format!("{audit_retention}/holds/")))
        || path == format!("{audit_retention}/purge")
    {
        let repository = match repository(shared) {
            Ok(repository) => repository,
            Err(response) => return Some(response),
        };
        return Some(audit_retention_response(
            captured,
            path,
            &audit_retention,
            shared,
            admin_auth,
            base_action,
            &repository,
        ));
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
    let break_glass_approvals = format!("{admin_prefix}/break-glass-approvals");
    if path == break_glass_approvals || path.starts_with(&(break_glass_approvals.clone() + "/")) {
        let repository = match repository(shared) {
            Ok(repository) => repository,
            Err(response) => return Some(response),
        };
        return Some(break_glass_approval_response(
            captured,
            path,
            &break_glass_approvals,
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

    fn list_approvals(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: ApprovalKind,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_approvals(tenant_id, kind),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_approvals(tenant_id, kind)),
        }
    }

    fn upsert_audit_legal_hold(
        &self,
        hold: &AuditRetentionHold,
        created_by: prodex_domain::PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.upsert_audit_legal_hold(hold, created_by, created_at_unix_ms, audit)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_upsert_audit_legal_hold(
                hold.clone(),
                created_by,
                created_at_unix_ms,
                audit,
            )),
        }
    }

    fn list_audit_legal_holds(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.list_audit_legal_holds(tenant_id),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_list_audit_legal_holds(tenant_id)),
        }
    }

    fn delete_audit_legal_hold(
        &self,
        tenant_id: prodex_domain::TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
    ) -> Result<bool, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => {
                repository.delete_audit_legal_hold(tenant_id, event_id, audit)
            }
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(
                repository.governance_delete_audit_legal_hold(tenant_id, event_id, audit),
            ),
        }
    }

    fn purge_audit_events(
        &self,
        tenant_id: prodex_domain::TenantId,
        event_ids: &[AuditEventId],
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        match self {
            Self::Sqlite(repository) => repository.purge_audit_events(
                tenant_id,
                event_ids,
                now_unix_ms,
                cutoff_unix_ms,
                audit,
            ),
            Self::Postgres {
                repository,
                runtime,
            } => runtime.block_on(repository.governance_purge_audit_events(
                tenant_id,
                event_ids.to_vec(),
                now_unix_ms,
                cutoff_unix_ms,
                audit,
            )),
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
        RuntimeGovernanceResource::ProviderRegistry => super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                artifact,
                &shared.provider,
                shared.provider_credential.as_ref(),
                shared.runtime_shared.runtime_config.governance.mode,
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

fn break_glass_approval_response(
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
        ("GET", []) => match repository.list_approvals(tenant_id, ApprovalKind::BreakGlass) {
            Ok(approvals) => runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "governance.break_glass_approval.list",
                    "data": approvals.into_iter().map(break_glass_approval_json).collect::<Vec<_>>(),
                }),
            ),
            Err(error) => repository_error(error),
        },
        ("POST", []) => {
            let body = match runtime_gateway_admin_json_body(captured) {
                Ok(body) => body,
                Err(response) => return response,
            };
            let Some(approval_id) = body.get("approval_id").and_then(serde_json::Value::as_str)
            else {
                return invalid_request();
            };
            let reason = match body
                .get("reason_code")
                .and_then(serde_json::Value::as_str)
                .map(ApprovalReasonCode::new)
            {
                Some(Ok(reason)) => reason,
                _ => return invalid_request(),
            };
            let Some(expires_at_unix_ms) = body
                .get("expires_at_unix_ms")
                .and_then(serde_json::Value::as_u64)
            else {
                return invalid_request();
            };
            let required_quorum = body
                .get("required_quorum")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(ApprovalKind::BreakGlass.minimum_quorum());
            let now = runtime_gateway_now_unix_ms();
            if expires_at_unix_ms <= now || expires_at_unix_ms > now.saturating_add(3_600_000) {
                return invalid_request();
            }
            let approval_id = match ApprovalId::new(approval_id.to_string()) {
                Ok(approval_id) => approval_id,
                Err(_) => return invalid_request(),
            };
            let scope = match ApprovalScope::new(format!("audit_retention:{}", reason.as_str())) {
                Ok(scope) => scope,
                Err(_) => return invalid_request(),
            };
            let fingerprint = match ApprovalFingerprint::new(artifact_fingerprint(
                format!("{}:{}:{}", tenant_id, scope.as_str(), expires_at_unix_ms).as_bytes(),
            )) {
                Ok(fingerprint) => fingerprint,
                Err(_) => return invalid_request(),
            };
            let execution = match execution(captured, path, admin_auth, base_action) {
                Ok(execution) => execution,
                Err(response) => return response,
            };
            let approval = match ApprovalRecord::pending(
                approval_id.clone(),
                tenant_id,
                ApprovalKind::BreakGlass,
                scope,
                fingerprint,
                execution.authorized_action.audit_event.principal_id,
                required_quorum,
                expires_at_unix_ms,
            ) {
                Ok(approval) => approval,
                Err(_) => return invalid_request(),
            };
            let audit = match control_plane_audit_command(
                repository,
                &execution.authorized_action,
                "governance.break_glass_approval.create",
                "break_glass_approval",
                Some(approval_id.as_str()),
            ) {
                Ok(audit) => audit,
                Err(error) => return repository_error(error),
            };
            match repository.create_approval(approval.clone(), audit) {
                Ok(outcome) => runtime_gateway_admin_json_response(
                    if outcome == GovernanceWriteOutcome::Applied {
                        201
                    } else {
                        200
                    },
                    serde_json::json!({
                        "approval": break_glass_approval_json(approval),
                        "replayed": outcome == GovernanceWriteOutcome::Replayed,
                    }),
                ),
                Err(error) => repository_error(error),
            }
        }
        ("GET", [approval_id]) => {
            let approval_id = match ApprovalId::new((*approval_id).to_string()) {
                Ok(approval_id) => approval_id,
                Err(_) => return invalid_request(),
            };
            match repository.get_approval(tenant_id, &approval_id) {
                Ok(approval) if approval.kind == ApprovalKind::BreakGlass => {
                    runtime_gateway_admin_json_response(200, break_glass_approval_json(approval))
                }
                Ok(_) => repository_error(GovernanceRepositoryError::NotFound),
                Err(error) => repository_error(error),
            }
        }
        ("POST", [approval_id, action @ ("votes" | "activate" | "revoke")]) => {
            break_glass_transition_response(
                captured,
                path,
                approval_id,
                action,
                admin_auth,
                base_action,
                repository,
            )
        }
        ("GET" | "POST", _) => build_runtime_proxy_json_error_response(
            404,
            "break_glass_approval_not_found",
            "break-glass approval was not found",
        ),
        _ => build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this break-glass route",
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn break_glass_transition_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    approval_id: &str,
    transition: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
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
    let action = match transition {
        "votes" => match body.get("decision").and_then(serde_json::Value::as_str) {
            Some("approve") => ApprovalAction::Approve,
            Some("reject") => ApprovalAction::Reject,
            Some("cancel") => ApprovalAction::Cancel,
            _ => return invalid_request(),
        },
        "activate" => ApprovalAction::Activate,
        "revoke" => ApprovalAction::Supersede,
        _ => return invalid_request(),
    };
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(approval_id) => approval_id,
        Err(_) => return invalid_request(),
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) if approval.kind == ApprovalKind::BreakGlass => {}
        Ok(_) => return repository_error(GovernanceRepositoryError::NotFound),
        Err(error) => return repository_error(error),
    }
    let action_label = match action {
        ApprovalAction::Approve => "approve",
        ApprovalAction::Reject => "reject",
        ApprovalAction::Cancel => "cancel",
        ApprovalAction::Activate => "activate",
        ApprovalAction::Supersede => "revoke",
        ApprovalAction::RollBack => return invalid_request(),
    };
    let audit = match control_plane_audit_command(
        repository,
        &execution.authorized_action,
        &format!("governance.break_glass_approval.{action_label}"),
        "break_glass_approval",
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let reason = match action {
        ApprovalAction::Reject => Some(ApprovalReasonCode::new("approval.rejected").unwrap()),
        ApprovalAction::Cancel => Some(ApprovalReasonCode::new("approval.cancelled").unwrap()),
        _ => None,
    };
    match repository.transition_approval_idempotent(
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
            operation: execution.atomic_write.operation,
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(ApprovalVoteMutationOutcome::Applied(approval)) => {
            runtime_gateway_admin_json_response(200, break_glass_approval_json(approval))
        }
        Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)) => runtime_gateway_admin_json_response(
            200,
            break_glass_approval_snapshot_json(&approval_id, snapshot),
        ),
        Err(error) => repository_error(error),
    }
}

fn break_glass_approval_json(approval: ApprovalRecord) -> serde_json::Value {
    let reason_code = approval.scope.as_str().strip_prefix("audit_retention:");
    serde_json::json!({
        "object": "governance.break_glass_approval",
        "approval_id": approval.id.as_str(),
        "scope": "audit_retention",
        "reason_code": reason_code,
        "state": approval_state(approval.state),
        "version": approval.version,
        "required_quorum": approval.effective_required_quorum(),
        "vote_count": approval.votes.len(),
        "expires_at_unix_ms": approval.expires_at_unix_ms,
        "activated_at_unix_ms": approval.activated_at_unix_ms,
    })
}

fn break_glass_approval_snapshot_json(
    approval_id: &ApprovalId,
    snapshot: ApprovalVoteSnapshot,
) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.break_glass_approval",
        "approval_id": approval_id.as_str(),
        "state": approval_state(snapshot.state),
        "version": snapshot.version,
        "required_quorum": snapshot.required_quorum,
        "vote_count": snapshot.vote_count,
        "expires_at_unix_ms": snapshot.expires_at_unix_ms,
        "activated_at_unix_ms": snapshot.activated_at_unix_ms,
    })
}

#[allow(clippy::too_many_arguments)]
fn audit_retention_response(
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
        return match repository.upsert_audit_legal_hold(
            &hold,
            execution.authorized_action.audit_event.principal_id,
            execution.atomic_write.completed_at_unix_ms,
            audit,
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
        return match repository.delete_audit_legal_hold(tenant_id, event_id, audit) {
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
    match repository.purge_audit_events(tenant_id, &event_ids, now_unix_ms, cutoff_unix_ms, audit) {
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
    let activate = || {
        repository.activate_revision(
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
                governance_artifact_is_valid(
                    shared,
                    tenant_id,
                    resource,
                    Some(revision_id),
                    artifact,
                )
            },
        )
    };
    let activation = match shared.governance_authority.as_ref() {
        Some(authority) => authority.commit_for_tenant(tenant_id, activate),
        None => activate(),
    };
    match activation {
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
            let Some(worker) = shared.gateway_observability.siem_worker.as_ref() else {
                return build_runtime_proxy_json_error_response(
                    503,
                    "governance_outbox_exporter_unavailable",
                    "SIEM outbox exporter is not configured",
                );
            };
            let tenant_id = base_action.tenant.tenant_id;
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
                        match prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)
                        {
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

fn control_plane_audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    audit_action: &str,
    resource_kind: &str,
    resource_id: Option<&str>,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action =
        AuditAction::try_new(audit_action).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    event.resource = AuditResource::new(
        resource_kind,
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
